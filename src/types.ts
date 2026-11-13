export type Storage = 'copy' | 'link';
export type Role = 'original' | 'caption-source' | 'reference';
export interface Asset { id: string; role: Role; storage: Storage; path: string; filename: string; sourceUrl?: string }
export interface WebsiteLink { id: string; url: string; title: string; notes: string }
export interface DocumentAttachment { id: string; filename: string; format: string; storage: Storage; path: string; title: string; notes: string; role: 'reference' | 'caption-source'; contentHash: string }
export interface DocumentText { schemaVersion: number; documentId: string; sourceHash: string; status: 'ready' | 'empty' | 'error' | 'missing' | 'changed'; text: string; warning: string | null; truncated: boolean }
export interface CaptionRevision { caption: string; savedAt: number }
export interface Item { schemaVersion: number; id: string; title: string; caption: string; captionHistory?: CaptionRevision[]; notes: string; createdAt: number; updatedAt: number; revision: number; sourceHash?: string; assets: Asset[]; links?: WebsiteLink[]; documents?: DocumentAttachment[] }
export interface Summary { id: string; title: string; caption: string; assetCount: number; missingOriginal: boolean; missingFiles: boolean; emptyCaption: boolean; thumbnailAsset: string | null }
export interface Page { items: Summary[]; total: number }
export interface Report { indexed: number; warnings: string[] }
export interface Info { root: string; settings: { schemaVersion: number; defaultStorage: Storage; captionInstructions?: string }; report?: Report }
export interface Missing { itemId: string; title: string; assetId: string; filename: string }
export type Filter = 'all' | 'missing-original' | 'missing-files' | 'empty-caption';
export interface AgentAccount { id: string; email: string; connected: boolean; planEnabled: boolean }
export interface AgentStatus { accounts: AgentAccount[]; active: string | null; model: string | null; connecting: boolean; lastError: string | null; sendOnlyAttachments: boolean }
export interface AgentModel { id: string; name: string }
export interface CaptionDraft { itemId: string; expectedHash: string; caption: string; model: string | null; replaceExisting?: boolean; history?: CaptionRevision[]; draftRevision?: number; stale?: boolean }
export interface QueueCounts { total: number; pending: number; processing: number; done: number; skippedCaption: number; skippedDraft: number; failed: number; cancelled: number }
export interface QueuePreview { scope: 'all' | 'selected'; prompt: string; counts: QueueCounts }
export interface QueueEntry { itemId: string; title: string; state: string; error: string | null }
export interface CaptionQueue { id: string; scope: string; prompt: string; model: string; state: 'running' | 'pausing' | 'paused' | 'cancelling' | 'cancelled' | 'completed'; lastError: string | null; counts: QueueCounts; entries: QueueEntry[] }
export interface BatchStatus { queue: CaptionQueue | null; draftCount: number; active: boolean }
export interface DraftSummary { itemId: string; title: string; caption: string; model: string; stale: boolean; draftRevision: number; createdAt: number }
export interface DraftPage { items: DraftSummary[]; total: number }
export interface ConversationTurn { id: string; question: string; answer: string | null; error: string | null; model: string; sourceHash: string; createdAt: number }
export interface Conversation { schemaVersion: number; itemId: string; revision: number; turns: ConversationTurn[] }
export interface ConversationView { conversation: Conversation; itemId: string; title: string; sourceHash: string; assetCount: number; documentCount?: number; linkCount?: number }
