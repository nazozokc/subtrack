import { render } from "ink"
import { App } from "./tui/app.tsx"

export async function handleTui(): Promise<void> {
  const { waitUntilExit } = render(<App />, {
    exitOnCtrlC: true,
    patchConsole: true,
  })

  try {
    await waitUntilExit()
  } catch (error) {
    // App exited with an error — silently ignore for clean exit
  }
}
