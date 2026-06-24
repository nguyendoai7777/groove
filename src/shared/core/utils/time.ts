/**
 * Formats a duration in seconds to a string representation (hh:mm:ss or mm:ss).
 *
 * @param secs The duration in seconds.
 * @returns Formatted duration string.
 */
export function formatDuration(secs: number): string {
  if (!secs || isNaN(secs)) return '0:00';

  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  const seconds = Math.floor(secs % 60);

  const secondsStr = seconds < 10 ? `0${seconds}` : seconds;

  if (hours > 0) {
    const minutesStr = minutes < 10 ? `0${minutes}` : minutes;
    return `${hours}:${minutesStr}:${secondsStr}`;
  }

  return `${minutes}:${secondsStr}`;
}
