/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** subtrack binary - Path to the subtrack executable. Leave empty to search your PATH. */
  "binaryPath"?: string,
  /** Database directory - Overrides where subtrack reads its database. Leave empty to use the default. */
  "dbDir"?: string,
  /** Display currency - Convert all prices to this currency. Leave unset to keep original currencies. */
  "defaultCurrency"?: "" | "USD" | "EUR" | "GBP" | "JPY" | "KRW" | "AUD" | "CAD" | "CNY" | "SGD" | "HKD"
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `subscriptions` command */
  export type Subscriptions = ExtensionPreferences & {}
  /** Preferences accessible in the `upcoming` command */
  export type Upcoming = ExtensionPreferences & {}
  /** Preferences accessible in the `summary` command */
  export type Summary = ExtensionPreferences & {}
  /** Preferences accessible in the `budget` command */
  export type Budget = ExtensionPreferences & {}
  /** Preferences accessible in the `add-subscription` command */
  export type AddSubscription = ExtensionPreferences & {}
  /** Preferences accessible in the `edit-subscription` command */
  export type EditSubscription = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `subscriptions` command */
  export type Subscriptions = {}
  /** Arguments passed to the `upcoming` command */
  export type Upcoming = {
  /** Days ahead (default 30) */
  "days": string
}
  /** Arguments passed to the `summary` command */
  export type Summary = {}
  /** Arguments passed to the `budget` command */
  export type Budget = {}
  /** Arguments passed to the `add-subscription` command */
  export type AddSubscription = {}
  /** Arguments passed to the `edit-subscription` command */
  export type EditSubscription = {}
}

