/**
 * The update check, and asking permission for it — PRODUCT §11, P6.3.
 *
 * "Opt-in on first run, with the prompt stating it is the application's only
 * network request."
 *
 * That sentence is the feature. The prompt is not a formality before a check
 * that was going to happen anyway — it is the only thing standing between this
 * application and a network request, and it has to say exactly what it is
 * asking for, because SECURITY §6 makes a promise the user cannot verify for
 * themselves: *this is the only request, ever*.
 *
 * # Never asked is not the same as refused
 *
 * The stored value is a tri-state. A boolean would make "has not been asked"
 * and "said no" the same value, and the two need different behaviour: one
 * prompts, the other never mentions it again. They are only the same in that
 * neither permits a request.
 *
 * # And never during an unsaved edit
 *
 * §11: "Never installs without consent or during an unsaved edit." The prompt
 * itself waits too. A modal appearing over a document somebody is typing into
 * gets dismissed by the next keystroke, which is consent nobody gave.
 */

import { read, write } from "./settings";

export type Consent = "unasked" | "allowed" | "refused";

const KEY = "update-consent";

const isConsent = (value: unknown): value is Consent =>
  value === "unasked" || value === "allowed" || value === "refused";

/** What a check produced, as the shell reports it. */
export type Outcome =
  | { state: "compiled-out" }
  | { state: "not-permitted"; consent: Consent }
  | { state: "up-to-date" }
  | { state: "available"; version: string; notes: string }
  | { state: "failed"; reason: string };

class Updates {
  /** What the user has said, if anything. */
  consent = $state<Consent>(read(KEY, "unasked", isConsent));

  /** Whether this build can check at all. Off until the shell says otherwise. */
  possible = $state(false);

  /** The last result, for the interface to show. */
  outcome = $state<Outcome | null>(null);

  /** Whether the first-run prompt should be showing. */
  asking = $state(false);

  /**
   * Asks the shell whether an update check is even possible.
   *
   * The offline variant compiles it out, and an interface that offers a switch
   * doing nothing is worse than one that offers no switch.
   */
  async inspect(invoke: (command: string) => Promise<unknown>): Promise<void> {
    try {
      this.possible = (await invoke("updates_possible")) === true;
    } catch {
      // A shell that cannot answer is one that cannot check either.
      this.possible = false;
    }
  }

  /**
   * Raises the first-run prompt, if this is the first run and nothing is
   * unsaved.
   *
   * Deferred rather than dropped when the document is dirty: the question is
   * still worth asking, just not over somebody's typing.
   */
  askIfNeeded(dirty: boolean): void {
    if (!this.possible || this.consent !== "unasked" || dirty) return;
    this.asking = true;
  }

  /** Records an answer and stops asking. */
  answer(consent: Consent): void {
    this.consent = consent;
    this.asking = false;
    write(KEY, consent);
  }

  /**
   * Checks, if permitted.
   *
   * The consent is sent with the request rather than read on the other side,
   * so there is one copy of the answer and it is the one the user gave.
   */
  async check(
    invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>,
  ): Promise<void> {
    if (!this.possible) {
      this.outcome = { state: "compiled-out" };
      return;
    }

    try {
      this.outcome = (await invoke("check_for_update", { consent: this.consent })) as Outcome;
    } catch (error) {
      this.outcome = {
        state: "failed",
        reason: error instanceof Error ? error.message : String(error),
      };
    }
  }
}

export const updates = new Updates();
