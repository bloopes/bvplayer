export function formatTime(secs: number): string {
  if (isNaN(secs)) return "0:00";
  const s = Math.floor(secs);
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`;
}