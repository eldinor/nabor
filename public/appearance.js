// Runs before the application stylesheet to restore appearance on first paint.
(() => {
  let preference = 'system';
  try { const saved = localStorage.getItem('nabor.appearance'); if (saved === 'light' || saved === 'dark') preference = saved; } catch {}
  const resolved = preference === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : preference;
  document.documentElement.dataset.appearance = preference;
  document.documentElement.dataset.theme = resolved;
  document.documentElement.style.colorScheme = resolved;
  document.documentElement.style.backgroundColor = resolved === 'dark' ? '#1c1e22' : '#fafafb';
})();
