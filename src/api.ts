import { invoke, convertFileSrc, isTauri } from '@tauri-apps/api/core';
import { open, confirm } from '@tauri-apps/plugin-dialog';
import { readImage, readText, writeText } from '@tauri-apps/plugin-clipboard-manager';

declare global { interface Window { __NABOR_TEST__?: { request: (payload: Record<string, unknown>) => Promise<unknown>; agent?: (payload: Record<string, unknown>) => Promise<unknown>; batch?: (payload: Record<string, unknown>) => Promise<unknown>; talk?: (payload: Record<string, unknown>) => Promise<unknown>; choose: (directory: boolean) => Promise<string[] | string | null>; confirm: () => Promise<boolean> } } }
const test = () => import.meta.env.DEV ? window.__NABOR_TEST__ : undefined;
export const native = () => isTauri() || !!test();
export async function request<T>(action: string, args: Record<string, unknown> = {}): Promise<T> {
  if (test()) return await test()!.request({ action, ...args }) as T;
  if (!isTauri()) throw new Error('Open Nabor as a desktop app to work with your files. Run npm run tauri dev.');
  return invoke<T>('request', { payload: { action, ...args } });
}
export async function agentRequest<T>(action: string, args: Record<string, unknown> = {}): Promise<T> {
  if (test()?.agent) return await test()!.agent!({ action, ...args }) as T;
  if (!isTauri()) throw new Error('ChatGPT connection requires the Nabor desktop app.');
  return invoke<T>('agent_request', { payload: { action, ...args } });
}
export async function batchRequest<T>(action: string, args: Record<string, unknown> = {}): Promise<T> {
  if (test()?.batch) return await test()!.batch!({ action, ...args }) as T;
  if (test()) { if (action === 'status') return { queue: null, draftCount: 0, active: false } as T; if (action === 'drafts') return { items: [], total: 0 } as T; }
  if (!isTauri()) throw new Error('Caption queues require the Nabor desktop app.');
  return invoke<T>('batch_request', { payload: { action, ...args } });
}
export async function talkRequest<T>(action: string, args: Record<string, unknown> = {}): Promise<T> {
  if (test()?.talk) return await test()!.talk!({ action, ...args }) as T;
  if (!isTauri()) throw new Error('Agent conversations require the Nabor desktop app.');
  return invoke<T>('talk_request', { payload: { action, ...args } });
}
export async function copyText(text: string) {
  if (test()) return navigator.clipboard.writeText(text);
  return writeText(text);
}
export async function confirmClearConversation() {
  return test() ? test()!.confirm() : confirm('Delete this item’s saved questions and replies?', { title: 'Clear conversation?', kind: 'warning' });
}
export async function choose(directory = false, kind: 'all' | 'images' | 'documents' = 'all'): Promise<string[] | string | null> {
  if (test()) return test()!.choose(directory);
  if (!isTauri()) throw new Error('File access is available in the Nabor desktop app. Run npm run tauri dev.');
  const images = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff'];
  const documents = ['pdf', 'docx', 'md', 'txt'];
  return open({ directory, multiple: !directory, filters: directory ? undefined : [{ name: kind === 'images' ? 'Images' : kind === 'documents' ? 'Documents' : 'Images and documents', extensions: kind === 'images' ? images : kind === 'documents' ? documents : [...images, ...documents] }] });
}
export async function confirmation(message: string) { return test() ? test()!.confirm() : confirm(message, { title: 'Remove from collection?', kind: 'warning' }); }
export async function confirmDiscardCaption() { return confirm('Discard the open caption draft and close Nabor?', { title: 'Unsaved caption draft', kind: 'warning' }); }
export const imageUrl = (path: string) => path.startsWith('data:') ? path : convertFileSrc(path);
export async function clipboard(): Promise<{ rgba: number[]; width: number; height: number } | string> {
  try {
    const img = await readImage();
    try { const { width, height } = await img.size(); return { width, height, rgba: Array.from(await img.rgba()) }; }
    finally { await img.close(); }
  } catch { const text = await readText(); if (!text) throw new Error('Copy an image or caption text first.'); return text; }
}
export async function blobPixels(blob: Blob) {
  const bitmap = await createImageBitmap(blob);
  try { const canvas = document.createElement('canvas'); canvas.width = bitmap.width; canvas.height = bitmap.height; const context = canvas.getContext('2d')!; context.drawImage(bitmap, 0, 0); return { width: bitmap.width, height: bitmap.height, rgba: Array.from(context.getImageData(0, 0, bitmap.width, bitmap.height).data) }; }
  finally { bitmap.close(); }
}
