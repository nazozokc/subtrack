import { render } from "ink"
import { consola } from "consola"
import { App } from "./tui/app.tsx"

export async function handleTui(): Promise<void> {
  // Silence consola during TUI to prevent log pollution
  const prevLevel = consola.level
  consola.level = -999

  let instance: ReturnType<typeof render> | undefined
  try {
    instance = render(<App />, {
      exitOnCtrlC: true,
      patchConsole: true,
    })
    await instance.waitUntilExit()
  } catch {
    // App exited with an error — ignore for clean exit
  } finally {
    // Clear Ink's output and restore terminal
    try {
      instance?.clear()
    } finally {
      consola.level = prevLevel
    }
  }
}
