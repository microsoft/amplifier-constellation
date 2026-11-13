'use strict';
const video = document.getElementById('story');
const chapters = [...document.querySelectorAll('[data-time]')];
if (video) {
  let pending = null;
  function markChapter() {
    const time = video.currentTime + 0.001;
    chapters.forEach((button, i) => {
      const next = chapters[i + 1];
      if (time >= Number(button.dataset.time) && (!next || time < Number(next.dataset.time))) button.setAttribute('aria-current', 'true');
      else button.removeAttribute('aria-current');
    });
  }
  chapters.forEach(button => button.addEventListener('click', () => {
    const time = Number(button.dataset.time);
    if (video.readyState >= 1) video.currentTime = time;
    else { pending = time; video.load(); }
    markChapter();
  }));
  video.addEventListener('loadedmetadata', () => {
    if (pending !== null) { video.currentTime = pending; pending = null; }
    markChapter();
  });
  video.addEventListener('timeupdate', markChapter);
  video.addEventListener('seeked', markChapter);
  markChapter();
}
