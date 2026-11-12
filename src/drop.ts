export interface ImageDrop { files: File[]; urls: string[]; sourceUrl: string | null; localPaths: string[] }
const webUrl = (text: string): string | null => {
  try { const url = new URL(text); return ['http:', 'https:'].includes(url.protocol) && !url.username && !url.password ? url.href : null; } catch { return null; }
};
export function readImageDrop(data: DataTransfer): ImageDrop {
  // Parse detached markup; never inject dragged HTML into the application.
  const html = data.getData('text/html');
  const image = html ? new DOMParser().parseFromString(html, 'text/html').querySelector('img')?.getAttribute('src') : null;
  const imageUrl = image ? webUrl(image) : null;
  const downloadUrl = data.getData('DownloadURL').match(/^[^:]+:[^:]*:(https?:\/\/.+)$/s)?.[1];
  const uriList = data.getData('text/uri-list').split(/\r?\n/).map(s => s.trim()).filter(s => s && !s.startsWith('#'));
  const localPaths = uriList.filter(s => s.startsWith('file:')).map(s => {
    const url = new URL(s);
    if (url.hostname && url.hostname !== 'localhost') throw new Error('Use the file picker to add files from a network share.');
    const path = decodeURIComponent(url.pathname); return /^\/[a-z]:\//i.test(path) ? path.slice(1) : path;
  });
  const candidates = imageUrl ? [imageUrl] : downloadUrl && webUrl(downloadUrl) ? [webUrl(downloadUrl)!] : uriList.map(webUrl).filter((s): s is string => !!s);
  if (!candidates.length) { const plain = webUrl(data.getData('text/plain').trim()); if (plain) candidates.push(plain); }
  const urls = [...new Set(candidates)];
  const files = Array.from(data.files);
  if (files.some(f => f.size > 50 * 1024 * 1024)) throw new Error('Image exceeds the 50 MB import limit.');
  if (Math.max(files.length, urls.length, localPaths.length) > 20) throw new Error('Drop up to 20 images at a time.');
  if (!files.length && !urls.length && !localPaths.length) throw new Error('Drop an image or a direct image URL. Images available only inside Chrome must be saved or copied first.');
  return { files, urls, sourceUrl: imageUrl ?? (downloadUrl ? webUrl(downloadUrl) : null) ?? urls[0] ?? null, localPaths };
}
export const fileBase64 = (file: File) => new Promise<string>((resolve, reject) => {
  const reader = new FileReader(); reader.onerror = () => reject(new Error('Cannot read the dropped image.'));
  reader.onload = () => resolve(String(reader.result).split(',')[1]); reader.readAsDataURL(file);
});
