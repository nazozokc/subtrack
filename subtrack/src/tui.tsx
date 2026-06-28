import { render } from "ink"
import { consola } from "consola"
import { App } from "./tui/app.tsx"

export async function handleTui(): Promise<void> {
  // Silence consola during TUI to prevent log pollution
  const prevLevel = consola.level
  consola.level = -999

  const instance = render(<App />, {
    exitOnCtrlC: true,
    patchConsole: true,
  })

  try {
    await instance.waitUntilExit()
  } catch {
    // App exited with an error — silently ignore for clean exit
  } finally {
    // Clear Ink's output and restore terminal
    instance.clear()
    consola.level = prevLevel
  }
}
