#!/usr/bin/env node
import { Command } from "commander"
import { handleList, handleAdd, handleDelete, handleTags, handleBackup } from "./commands.ts"

const runCLI = () => {
  const program = new Command()
  program.name("subtrack")

  program
    .command("list")
    .option("-c, --currency <currency>", "convert all prices to currency")
    .action(handleList)

  program
    .command("add")
    .option("--name <name>", "subscription name")
    .option("--price <price>", "monthly payment amount")
    .option("--currency <currency>", "currency")
    .option("--cycle <cycle>", "billing cycle")
    .option("--tags <tags>", "comma-separated tags")
    .action(handleAdd)

  program.command("delete").action(handleDelete)

  program
    .command("tags")
    .argument("<taglist...>")
    .action(handleTags)

  program
    .command("backup")
    .argument("<destination>", "backup destination directory")
    .action(handleBackup)

  program.parse()
}

runCLI()
