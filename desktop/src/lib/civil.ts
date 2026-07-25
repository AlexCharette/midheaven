// Pure civil-instant math — the one home for the calculator's calendar
// arithmetic. The canonical value is MINUTES since the civil epoch
// 0000-03-01 (proleptic Gregorian, Hinnant's days_from_civil), unbounded in
// both directions, so ring drags accumulate across midnights and new years
// with no special cases. Wall-clock only: time zones and DST live in the
// backend (`preview` resolves them per moment and reports gaps/folds).

export type Moment = { date: string; time: string }; // "YYYY-MM-DD", "HH:MM"

export const MINUTES_PER_DAY = 1440;

export const isLeap = (y: number): boolean => y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);

export const daysInYear = (y: number): number => (isLeap(y) ? 366 : 365);

const MONTH_DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/** Days in month `m` (1–12) of year `y`. */
export const daysInMonth = (y: number, m: number): number =>
  m === 2 && isLeap(y) ? 29 : MONTH_DAYS[m - 1];

/** 1-based ordinal day of the year (Jan 1 → 1, Dec 31 → 365/366). */
export function dayOfYear(y: number, m: number, d: number): number {
  let n = d;
  for (let i = 1; i < m; i++) n += daysInMonth(y, i);
  return n;
}

/** Civil day number for Y-M-D (Hinnant's days_from_civil; 1970-01-01 → 719468). */
export function toDays(y: number, m: number, d: number): number {
  const yy = m <= 2 ? y - 1 : y;
  const era = Math.floor(yy / 400);
  const yoe = yy - era * 400;
  const doy = Math.floor((153 * (m + (m > 2 ? -3 : 9)) + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe;
}

/** Inverse of {@link toDays}. */
export function fromDays(z: number): { y: number; m: number; d: number } {
  const era = Math.floor(z / 146097);
  const doe = z - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const m = mp < 10 ? mp + 3 : mp - 9;
  return { y: yoe + era * 400 + (m <= 2 ? 1 : 0), m, d };
}

/** Parse "YYYY-MM-DD" into a real calendar date; null for shape or calendar
 * violations (Feb 30, month 13, 2023-02-29). */
export function parseDate(s: string): { y: number; m: number; d: number } | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s.trim());
  if (!match) return null;
  const [y, m, d] = [Number(match[1]), Number(match[2]), Number(match[3])];
  if (m < 1 || m > 12 || d < 1 || d > daysInMonth(y, m)) return null;
  return { y, m, d };
}

/** Parse "HH:MM" (24h; seconds not accepted — the fields' contract) into
 * minutes of day, or null. */
export function parseTime(s: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(s.trim());
  if (!match) return null;
  const [hh, mm] = [Number(match[1]), Number(match[2])];
  if (hh > 23 || mm > 59) return null;
  return hh * 60 + mm;
}

/** The canonical instant: civil minutes for a date + time pair, or null when
 * either part is invalid. */
export function toMinutes(date: string, time: string): number | null {
  const d = parseDate(date);
  const t = parseTime(time);
  if (d === null || t === null) return null;
  return toDays(d.y, d.m, d.d) * MINUTES_PER_DAY + t;
}

const pad = (n: number, w = 2) => String(n).padStart(w, "0");

/** Render civil minutes back to canonical field strings. */
export function fromMinutes(min: number): Moment {
  const days = Math.floor(min / MINUTES_PER_DAY);
  const rest = min - days * MINUTES_PER_DAY;
  const { y, m, d } = fromDays(days);
  return {
    date: `${pad(y, 4)}-${pad(m)}-${pad(d)}`,
    time: `${pad(Math.floor(rest / 60))}:${pad(rest % 60)}`,
  };
}

/** The current local wall-clock moment. */
export function nowMoment(): Moment {
  const now = new Date();
  return {
    date: `${pad(now.getFullYear(), 4)}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`,
    time: `${pad(now.getHours())}:${pad(now.getMinutes())}`,
  };
}

/** Move a date to another year keeping month/day; Feb 29 clamps to Feb 28. */
export function setYear(date: string, year: number): string {
  const d = parseDate(date);
  if (!d) return date;
  const day = Math.min(d.d, daysInMonth(year, d.m));
  return `${pad(year, 4)}-${pad(d.m)}-${pad(day)}`;
}

/** Shift an instant by whole months keeping day-of-month (clamped to the
 * target month's length) and time of day — the date ring's coarse keyboard
 * step. */
export function addMonths(min: number, delta: number): number {
  const days = Math.floor(min / MINUTES_PER_DAY);
  const rest = min - days * MINUTES_PER_DAY;
  const { y, m, d } = fromDays(days);
  const months = y * 12 + (m - 1) + delta;
  const ty = Math.floor(months / 12);
  const tm = months - ty * 12 + 1;
  return toDays(ty, tm, Math.min(d, daysInMonth(ty, tm))) * MINUTES_PER_DAY + rest;
}

/** The two ring rotations for an instant, degrees clockwise of the fixed
 * index: the time ring turns once per day, the date ring once per (leap-aware)
 * year, geared continuously — the date hand creeps as the time hand turns. */
export function ringAngles(min: number): { timeAngle: number; dateAngle: number } {
  const days = Math.floor(min / MINUTES_PER_DAY);
  const minutesOfDay = min - days * MINUTES_PER_DAY;
  const { y, m, d } = fromDays(days);
  const doy0 = dayOfYear(y, m, d) - 1;
  return {
    timeAngle: (minutesOfDay / MINUTES_PER_DAY) * 360,
    dateAngle: ((doy0 + minutesOfDay / MINUTES_PER_DAY) / daysInYear(y)) * 360,
  };
}

/** Month names for the date ring's limb lettering. */
export const MONTH_NAMES = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];
