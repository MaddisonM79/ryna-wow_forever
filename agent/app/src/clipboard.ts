// Copying to the clipboard from the window. The webview's clipboard can refuse (no permission,
// not a secure context), so this says whether it worked and the caller falls back to "select it".

export async function copyText(text: string): Promise<boolean> {
  try {
    if (!navigator.clipboard?.writeText) return false;
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** The activity log as plain text, oldest first: "09:02 Pushed Main: 4 characters". */
export function logText(lines: { at: number; text: string }[], time: (at: number) => string): string {
  return [...lines]
    .reverse()
    .map((l) => `${time(l.at)} ${l.text}`)
    .join("\n");
}
