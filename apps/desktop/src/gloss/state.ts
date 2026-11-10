// What the Gloss panel should offer for the selected segment. Pure: no DOM.
import { forms } from "./tokenizer.ts";

/** `empty`: nothing typed, nothing to break. `untokenized`: words typed, none broken out yet.
 * `current`: the tokens are the words of the transcription. `changed`: they are not. */
export type TokenState = "empty" | "untokenized" | "current" | "changed";

/** Compares forms joined against the tokenizer's reading of the transcription, under NFC. */
export function tokenState(content: string, tokenForms: readonly string[]): TokenState {
  const typed = forms(content);
  if (tokenForms.length === 0) return typed.length === 0 ? "empty" : "untokenized";
  const same = typed.length === tokenForms.length && typed.every((f, i) => f === tokenForms[i]!.normalize("NFC"));
  return same ? "current" : "changed";
}

export const transcriptionChanged = (content: string, tokenForms: readonly string[]): boolean => tokenState(content, tokenForms) === "changed";
