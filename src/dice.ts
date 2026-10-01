import type { DiceKind, DiceResult } from "./types";

export interface DiceFace { key: string; name: string; symbol: string; brief: string; kind: DiceKind; label: string; prompt: string }

const build = (kind: DiceKind, label: string, prompt: string, rows: [string, string, string, string][]): DiceFace[] => rows.map(([key, name, symbol, brief]) => ({ key, name, symbol, brief, kind, label, prompt }));

export const planets = build("planet", "行星骰", "能量的主角", [
  ["sun", "太阳", "☉", "意志与生命力，自我表达的核心"], ["moon", "月亮", "☽", "情绪与直觉，内在安全感"], ["mercury", "水星", "☿", "沟通与思维，信息的流动"], ["venus", "金星", "♀", "爱与美，关系与价值"], ["mars", "火星", "♂", "行动与欲望，争取的方式"], ["jupiter", "木星", "♃", "扩张与机遇，成长的方向"], ["saturn", "土星", "♄", "限制与责任，课题所在"], ["uranus", "天王星", "♅", "突变与革新，意外变量"], ["neptune", "海王星", "♆", "灵感与迷雾，模糊地带"], ["pluto", "冥王星", "♇", "转化与执念，深层力量"], ["north-node", "北交点", "☊", "前进方向，此生要学的功课"], ["south-node", "南交点", "☋", "惯性与过去，熟悉的旧路"]
]);
export const zodiacs = build("zodiac", "星座骰", "呈现的方式", [
  ["aries", "白羊座", "♈", "开创与冲动，直接行动"], ["taurus", "金牛座", "♉", "稳定与积累，感官的享受"], ["gemini", "双子座", "♊", "交流与多变，信息的交换"], ["cancer", "巨蟹座", "♋", "情绪与守护，家庭的归属"], ["leo", "狮子座", "♌", "表现与创造，成为焦点"], ["virgo", "处女座", "♍", "分析与完善，细节的秩序"], ["libra", "天秤座", "♎", "关系与权衡，寻求平衡"], ["scorpio", "天蝎座", "♏", "深刻与执着，洞察真相"], ["sagittarius", "射手座", "♐", "探索与信念，放眼远方"], ["capricorn", "摩羯座", "♑", "目标与纪律，务实攀登"], ["aquarius", "水瓶座", "♒", "独立与前瞻，打破常规"], ["pisces", "双鱼座", "♓", "想象与共情，消融边界"]
]);
export const houses = build("house", "宫骰", "发生的领域", [
  ["house-1", "第 1 宫", "Ⅰ", "自我形象，给人的第一印象"], ["house-2", "第 2 宫", "Ⅱ", "金钱与价值，拥有的资源"], ["house-3", "第 3 宫", "Ⅲ", "沟通与学习，身边的环境"], ["house-4", "第 4 宫", "Ⅳ", "家庭与根基，内在的归属"], ["house-5", "第 5 宫", "Ⅴ", "恋爱与创造，让人快乐的事"], ["house-6", "第 6 宫", "Ⅵ", "日常与健康，工作的习惯"], ["house-7", "第 7 宫", "Ⅶ", "伴侣与合作，一对一关系"], ["house-8", "第 8 宫", "Ⅷ", "共享与深层，他人的资源"], ["house-9", "第 9 宫", "Ⅸ", "远方与理念，学习的进阶"], ["house-10", "第 10 宫", "Ⅹ", "事业与名望，公众的形象"], ["house-11", "第 11 宫", "Ⅺ", "朋友与愿景，群体的关系"], ["house-12", "第 12 宫", "Ⅻ", "隐秘与沉淀，潜意识的海"]
]);

export const diceGroups: Record<DiceKind, DiceFace[]> = { planet: planets, zodiac: zodiacs, house: houses };
export const faceOf = (kind: DiceKind, key: string) => diceGroups[kind].find((x) => x.key === key) ?? diceGroups[kind][0];
const secureIndex = (length: number) => { if (!globalThis.crypto?.getRandomValues) throw new Error("当前运行环境不支持安全随机源。"); const limit = Math.floor(0x1_0000_0000 / length) * length; const value = new Uint32Array(1); do { globalThis.crypto.getRandomValues(value); } while (value[0] >= limit); return value[0] % length; };
export const randomFace = (kind: DiceKind) => { const list = diceGroups[kind]; return list[secureIndex(list.length)]; };
export const randomDice = (): DiceResult => ({ planet: randomFace("planet").key, zodiac: randomFace("zodiac").key, house: randomFace("house").key });
export const diceContext = (dice: DiceResult) => ["planet", "zodiac", "house"].map((kind) => { const f = faceOf(kind as DiceKind, dice[kind as DiceKind]); return `${f.label}：${f.name}（${f.symbol}，关键词：${f.brief}）`; }).join("；");
export const diceText = (dice: DiceResult) => ["planet", "zodiac", "house"].map((kind) => { const f = faceOf(kind as DiceKind, dice[kind as DiceKind]); return `${f.label}：${f.name}（${f.symbol}）`; }).join("；");
export const diceNames = (dice: DiceResult) => [faceOf("planet", dice.planet).name, faceOf("zodiac", dice.zodiac).name, faceOf("house", dice.house).name].join(" · ");
export const normalizeDice = (input: Partial<Record<DiceKind, string>>): DiceResult => {
  const normalize = (kind: DiceKind) => { const raw = String(input[kind] ?? "").toLowerCase(); const exact = diceGroups[kind].find((f) => f.key.toLowerCase() === raw); if (exact) return exact.key; const contains = diceGroups[kind].find((f) => raw.includes(f.name) || raw.includes(f.key)); if (contains) return contains.key; if (kind === "house") { const n = raw.match(/(\d{1,2})/); if (n && Number(n[1]) >= 1 && Number(n[1]) <= 12) return `house-${n[1]}`; } return randomFace(kind).key; };
  return { planet: normalize("planet"), zodiac: normalize("zodiac"), house: normalize("house") };
};
