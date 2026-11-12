#!/usr/bin/env python3
"""Qualify the pinned private interpreter's ordinary public HTTPS trust.

Only a fixed read-only GitHub request is made. No custom CA, system installation,
credentials, model calls, or application state is used.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

import python_support

PROBE = r'''
import hashlib, json, os, platform, shutil, ssl, sys, urllib.error, urllib.request
url = "https://api.github.com"
result = {
    "url": url, "python": sys.version, "executable": sys.executable,
    "executable_sha256": hashlib.file_digest(open(sys.executable, "rb"), "sha256").hexdigest(),
    "machine": platform.machine(), "platform": platform.platform(),
    "openssl": ssl.OPENSSL_VERSION,
    "default_verify_paths": ssl.get_default_verify_paths()._asdict(),
    "isolated": bool(sys.flags.isolated), "bytecode_disabled": bool(sys.dont_write_bytecode),
    "utf8_mode": bool(sys.flags.utf8_mode),
    "ca_override_absent": all(name not in os.environ for name in ("SSL_CERT_FILE", "SSL_CERT_DIR")),
    "tools_on_path": [name for name in ("python", "python3", "git", "cargo", "node") if shutil.which(name)],
    "proxy_environment_present": any(os.environ.get(name) for name in ("HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy")),
}
try:
    context = ssl.create_default_context()
    result.update(verify_mode=int(context.verify_mode), check_hostname=context.check_hostname,
                  cert_store_before=context.cert_store_stats())
    assert context.verify_mode == ssl.CERT_REQUIRED and context.check_hostname
    assert result["isolated"] and result["bytecode_disabled"] and result["utf8_mode"]
    assert result["ca_override_absent"] and not result["tools_on_path"]
    request = urllib.request.Request(url, headers={"User-Agent": "Constellation-private-Python-qualification", "Accept": "application/vnd.github+json"})
    with urllib.request.urlopen(request, context=context, timeout=30) as response:
        result.update(http_status=response.status, final_url=response.geturl())
        assert response.status == 200 and response.geturl() in (url, url + "/")
        data = response.read(65537)
        assert len(data) <= 65536
        assert json.loads(data)["current_user_url"] == "https://api.github.com/user"
    result.update(cert_store_after=context.cert_store_stats(), outcome="passed")
except Exception as error:
    # Keep useful verification diagnostics without logging proxy addresses,
    # arbitrary response bodies, or process environment/credentials.
    reason = getattr(error, "reason", error)
    result.update(outcome="failed", error_type=type(error).__name__, reason_type=type(reason).__name__)
    if isinstance(reason, ssl.SSLCertVerificationError):
        result.update(verify_code=reason.verify_code, verify_message=reason.verify_message)
    if isinstance(error, urllib.error.HTTPError):
        result["http_status"] = error.code
print(json.dumps(result))
sys.exit(0 if result["outcome"] == "passed" else 1)
'''


def qualify(target, cache, output):
    lock, asset = python_support.asset_for(target)
    record = {
        'target': target, 'python_version': lock['python_version'],
        'archive_sha256': asset['sha256'],
        'lock_sha256': python_support.digest(python_support.ROOT / 'lock.json'),
        'profile': 'default verified HTTPS; no CA overrides or developer tools on child PATH',
        'model_calls': 0, 'application_operations': 0,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    try:
        with tempfile.TemporaryDirectory(prefix='Constellation private Python HTTPS ') as temporary:
            root = Path(temporary)
            record['phase'] = 'prepare'
            python_support.prepare(target, cache, root / 'python')
            executable = (root / 'python' / asset['executable']).resolve()
            empty_path = root / 'no-tools'; empty_path.mkdir()
            env = dict(os.environ, PATH=str(empty_path), PYTHONHOME=str(root / 'not-python'),
                       PYTHONPATH=str(root / 'not-python'))
            for name in ('SSL_CERT_FILE', 'SSL_CERT_DIR', 'REQUESTS_CA_BUNDLE', 'CURL_CA_BUNDLE'):
                env.pop(name, None)
            record['phase'] = 'private_interpreter_https'
            child = subprocess.run([str(executable), '-I', '-B', '-X', 'utf8', '-c', PROBE],
                                   env=env, cwd=root, capture_output=True, text=True,
                                   encoding='utf-8', timeout=45)
            record['probe'] = json.loads(child.stdout)
            assert Path(record['probe']['executable']).samefile(executable)
            assert record['probe']['executable_sha256'] == python_support.digest(executable)
            architectures = ('aarch64', 'arm64') if target.startswith('aarch64-') else ('x86_64', 'amd64')
            assert record['probe']['machine'].lower() in architectures, 'Interpreter architecture differs from target'
            if child.returncode:
                raise RuntimeError('Private interpreter HTTPS qualification failed')
            record['outcome'] = 'passed'
    except Exception as error:
        record.update(outcome='failed', failure_type=type(error).__name__)
        raise
    finally:
        output.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'target': target, 'outcome': record['outcome'], 'report': str(output)}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', required=True)
    parser.add_argument('--cache', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    qualify(args.target, args.cache, args.output)
