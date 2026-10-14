import { errorMessage } from "./error-message.ts";
import { showError } from "./show-error.ts";

export function reportFailure(call: Promise<unknown>): Promise<void> {
  return call.then(
    () => undefined,
    (error: unknown) => showError(errorMessage(error)),
  );
}
