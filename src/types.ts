export type DiceKind = "planet" | "zodiac" | "house";

export interface DiceResult { planet: string; zodiac: string; house: string }
export interface DiceReadingCard { diceLabel: string; faceName: string; faceGlyph: string; meaning: string }
export interface DivinationReading { diceReadings: DiceReadingCard[]; summary: string; imagePrompt: string }
export interface DivinationRecord { id: string; uid: string; created: string; updated: string; question: string; dice: DiceResult; reading: DivinationReading | null; summaryText: string; imageUrl: string; reviewNote: string; rating: number; tagNames: string[] }
export interface ComparisonRow { diceLabel: string; userReading: string; refReading: string }
export interface TrainingComparison { rows: ComparisonRow[]; highlights: string; improvements: string; overallComment: string; score: number }
export interface TrainingRecord { id: string; uid: string; created: string; updated: string; question: string; dice: DiceResult | null; diceText: string; userAnswer: string; aiCompareText: string; aiCompare: TrainingComparison | null; aiScore: number }
export interface GalleryItem { id: string; uid: string; created: string; updated: string; imageUrl: string; mode: "dice" | "poster" | string; question: string; diceText: string }
export interface Tag { id: string; name: string; color: string; meaning: string; created: string; updated: string }
export interface ModelProfile { id: string; name: string; capability: "text" | "image"; protocol: "openai-chat" | "openai-images" | string; endpoint: string; model: string; authType: "bearer" | "x-api-key" | "none" | string; extraHeaders: string; extraParams: string; timeoutSeconds: number; imageSizeOverride: string; imageQualityOverride: string; imageOutputFormatOverride: string; imageResponseFormatOverride: string; hasSecret: boolean; created: string; updated: string }
export interface ModelProfileInput { id?: string; name: string; capability: "text" | "image"; protocol: string; endpoint: string; model: string; authType: string; extraHeaders: string; extraParams: string; timeoutSeconds: number; imageSizeOverride: string; imageQualityOverride: string; imageOutputFormatOverride: string; imageResponseFormatOverride: string; apiKey?: string }
export interface Preferences { activeTextProfileId: string; activeImageProfileId: string; ttsVoiceShortName: string; imageDicePrompt: string; imagePosterPrompt: string; imageCustomPrompt: string; debugLoggingEnabled: boolean; debugLogContent: boolean; lastImageSaveDirectory: string; lastAudioSaveDirectory: string; trainingTopicScopes: string[]; trainingCustomScope: string }
export interface TtsVoice { shortName: string; displayName: string; localName: string; gender: string; locale: string; localeName: string }
export interface TtsAudio { audioBase64: string; mimeType: string; fileExtension: string }
export interface AppSnapshot { divinationRecords: DivinationRecord[]; trainingRecords: TrainingRecord[]; galleryItems: GalleryItem[]; tags: Tag[]; profiles: ModelProfile[]; prefs: Preferences; dataDirectory: string }
export interface MergeSummary { inserted: number; updated: number; skipped: number; tagsCreated: number; imagesCopied: number }
export interface AiResult { reading: DivinationReading; rawText: string }

export const emptySnapshot: AppSnapshot = { divinationRecords: [], trainingRecords: [], galleryItems: [], tags: [], profiles: [], prefs: { activeTextProfileId: "", activeImageProfileId: "", ttsVoiceShortName: "", imageDicePrompt: "", imagePosterPrompt: "", imageCustomPrompt: "", debugLoggingEnabled: false, debugLogContent: false, lastImageSaveDirectory: "", lastAudioSaveDirectory: "", trainingTopicScopes: [], trainingCustomScope: "" }, dataDirectory: "" };
