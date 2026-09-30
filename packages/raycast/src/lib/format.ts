/**
 * Presentation helpers. Formatting lives apart from the transport so a schema
 * change in subtrack shows up in one place instead of across every view.
 */
import { Icon } from "@raycast/api";
import type { Status, Subscription } from "./types";

export function formatPrice(amount: number, currency: string): string {
  // JPY and KRW are conventionally written without minor units.
  const fractionDigits = currency === "JPY" || currency === "KRW" ? 0 : 2;
  try {
    // No explicit locale: Raycast does not support localization, so the
    // numbers follow the system locale rather than a pinned one.
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency,
      minimumFractionDigits: fractionDigits,
      maximumFractionDigits: fractionDigits,
    }).format(amount);
  } catch {
    // An unknown currency code must not blank out the price.
    return `${amount.toFixed(fractionDigits)} ${currency}`;
  }
}

/** "3 days" / "today" — reads better in a list than a bare date. */
export function relativeDays(isoDate: string, today = new Date()): string {
  const target = new Date(`${isoDate}T00:00:00`);
  if (Number.isNaN(target.getTime())) return isoDate;
  const start = new Date(
    today.getFullYear(),
    today.getMonth(),
    today.getDate(),
  );
  const days = Math.round((target.getTime() - start.getTime()) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Tomorrow";
  if (days === -1) return "Yesterday";
  if (days > 0) return `in ${days} days`;
  return `${Math.abs(days)} days ago`;
}

export function formatDate(isoDate: string): string {
  const date = new Date(`${isoDate}T00:00:00`);
  if (Number.isNaN(date.getTime())) return isoDate;
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function statusLabel(status: Status): string {
  switch (status) {
    case "active":
      return "Active";
    case "paused":
      return "Paused";
    case "cancelled":
      return "Canceled";
    case "archived":
      return "Archived";
  }
}

/**
 * Raycast's `List.Item` icon takes a built-in `Icon` value and has no tint, so
 * status reads from the accompanying accessory text rather than color.
 */
const STATUS_ICON: Record<Status, Icon> = {
  active: Icon.CheckCircle,
  paused: Icon.Pause,
  cancelled: Icon.XmarkCircle,
  archived: Icon.Folder,
};

export function statusIcon(status: Status): Icon {
  return STATUS_ICON[status];
}

export function cycleLabel(cycle: string): string {
  switch (cycle) {
    case "bi-weekly":
      return "Every 2 weeks";
    case "semi-annual":
      return "Every 6 months";
    default:
      return cycle.charAt(0).toUpperCase() + cycle.slice(1);
  }
}

/** Turn a possibly-null field into a Detail row, dropping the ones that are empty. */
export function optionalField(
  label: string,
  value: string | number | boolean | null | undefined,
): {
  label: string;
  text: string;
} | null {
  if (value === null || value === undefined || value === "") return null;
  return { label, text: String(value) };
}

export function subscriptionFieldRows(
  sub: Subscription,
): { label: string; text: string }[] {
  const rows = [
    optionalField("Price", formatPrice(sub.price, sub.currency)),
    optionalField("Cycle", cycleLabel(sub.cycle)),
    optionalField("Status", statusLabel(sub.status)),
    optionalField("Tags", sub.tags.join(", ")),
    optionalField("Billing day", sub.billingDay),
    optionalField("Payment method", sub.paymentMethod),
    optionalField("Vendor", sub.vendorName),
    optionalField("Plan", sub.planTier),
    optionalField("Auto renew", sub.autoRenewal ? "Yes" : "No"),
    optionalField(
      "Discount",
      sub.discountAmount === null
        ? null
        : sub.discountType === "percentage"
          ? `${sub.discountAmount}%`
          : formatPrice(sub.discountAmount, sub.currency),
    ),
    optionalField(
      "Contract start",
      sub.contractStart && formatDate(sub.contractStart),
    ),
    optionalField(
      "Contract end",
      sub.contractEnd && formatDate(sub.contractEnd),
    ),
    optionalField("Added", formatDate(sub.createdAt)),
    optionalField("Notes", sub.notes),
  ];
  return rows.filter(
    (row): row is { label: string; text: string } => row !== null,
  );
}
