import type { JSX } from "react";
import { Action, Form, Icon, Toast, popToRoot, showToast } from "@raycast/api";
import { addSubscription, editSubscription } from "./subtrack";
import type { SubscriptionInput } from "./subtrack";
import type { Subscription } from "./types";

const CURRENCIES = [
  "USD",
  "EUR",
  "GBP",
  "JPY",
  "KRW",
  "AUD",
  "CAD",
  "CNY",
  "SGD",
  "HKD",
];

const CYCLES = [
  { title: "Weekly", value: "weekly" },
  { title: "Every 2 Weeks", value: "bi-weekly" },
  { title: "Monthly", value: "monthly" },
  { title: "Quarterly", value: "quarterly" },
  { title: "Every 6 Months", value: "semi-annual" },
  { title: "Yearly", value: "yearly" },
];

const STATUSES = [
  { title: "Active", value: "active" },
  { title: "Paused", value: "paused" },
  { title: "Cancelled", value: "cancelled" },
];

type TextFieldName =
  | "name"
  | "price"
  | "tags"
  | "paymentMethod"
  | "billingDay"
  | "vendorName"
  | "vendorUrl"
  | "planTier"
  | "notes";

const BASIC: { name: TextFieldName; title: string; placeholder: string }[] = [
  { name: "name", title: "Name", placeholder: "Netflix" },
  { name: "price", title: "Price", placeholder: "15.49" },
  { name: "tags", title: "Tags", placeholder: "video, streaming" },
];

const OPTIONAL: { name: TextFieldName; title: string; placeholder: string }[] =
  [
    {
      name: "paymentMethod",
      title: "Payment Method",
      placeholder: "credit_card",
    },
    { name: "billingDay", title: "Billing Day", placeholder: "1-31" },
    { name: "vendorName", title: "Vendor", placeholder: "Netflix Inc." },
    {
      name: "vendorUrl",
      title: "Vendor URL",
      placeholder: "https://netflix.com",
    },
    { name: "planTier", title: "Plan", placeholder: "Standard" },
    { name: "notes", title: "Notes", placeholder: "Shared with family" },
  ];

function text(
  subscription: Subscription | undefined,
  name: TextFieldName,
): string {
  const value = subscription?.[name];
  return value === null || value === undefined ? "" : String(value);
}

/**
 * Add and edit differ only in which command they call and whether fields start
 * populated, so one component covers both. `edit` passes only the fields the
 * user actually filled in, which is what `withValues` forwards as flags.
 */
export function SubscriptionForm({
  subscription,
  onSaved,
  navigationTitle,
}: {
  subscription?: Subscription;
  onSaved?: () => Promise<void>;
  navigationTitle?: string;
}): JSX.Element {
  const isEdit = subscription !== undefined;

  async function onSubmit(values: Form.Values): Promise<void> {
    const input = values as unknown as SubscriptionInput;

    // subtrack's own prompts need a TTY, so anything that would trip one has to
    // be caught here — an aborted process is a silent no-op for the user.
    const name = (input.name ?? "").trim();
    if (name === "") {
      await showToast({
        style: Toast.Style.Failure,
        title: "Name is required",
      });
      return;
    }
    const price = Number(input.price);
    if (!Number.isFinite(price) || price < 0) {
      await showToast({
        style: Toast.Style.Failure,
        title: "Price must be a number ≥ 0",
      });
      return;
    }
    if (input.billingDay !== undefined && input.billingDay !== "") {
      const day = Number(input.billingDay);
      if (!Number.isInteger(day) || day < 1 || day > 31) {
        await showToast({
          style: Toast.Style.Failure,
          title: "Billing day must be between 1 and 31",
        });
        return;
      }
    }

    try {
      if (isEdit) {
        await editSubscription(subscription.id, input);
        await showToast({
          style: Toast.Style.Success,
          title: `Updated ${name}`,
        });
      } else {
        await addSubscription(input);
        await showToast({ style: Toast.Style.Success, title: `Added ${name}` });
      }
    } catch (error) {
      await showToast({
        style: Toast.Style.Failure,
        title: error instanceof Error ? error.message : String(error),
      });
      return;
    }

    if (onSaved === undefined) {
      await popToRoot();
      return;
    }
    await onSaved();
  }

  return (
    <Form
      navigationTitle={navigationTitle}
      actions={
        <Action.SubmitForm
          title={isEdit ? "Save Changes" : "Add Subscription"}
          icon={isEdit ? Icon.Check : Icon.Plus}
          onSubmit={onSubmit}
        />
      }
    >
      <Form.Description
        text={
          isEdit
            ? "Change a field and save. Blank fields are left alone."
            : "Price is per billing cycle."
        }
      />

      {BASIC.map((field) => (
        <Form.TextField
          key={field.name}
          id={field.name}
          title={field.title}
          placeholder={field.placeholder}
          defaultValue={text(subscription, field.name)}
          autoFocus={field.name === "name"}
        />
      ))}

      <Form.Dropdown
        id="currency"
        title="Currency"
        defaultValue={subscription?.currency ?? "USD"}
      >
        {CURRENCIES.map((c) => (
          <Form.Dropdown.Item key={c} value={c} title={c} />
        ))}
      </Form.Dropdown>

      <Form.Dropdown
        id="cycle"
        title="Billing Cycle"
        defaultValue={subscription?.cycle ?? "monthly"}
      >
        {CYCLES.map((c) => (
          <Form.Dropdown.Item key={c.value} value={c.value} title={c.title} />
        ))}
      </Form.Dropdown>

      <Form.Dropdown
        id="status"
        title="Status"
        defaultValue={subscription?.status ?? "active"}
      >
        {STATUSES.map((s) => (
          <Form.Dropdown.Item key={s.value} value={s.value} title={s.title} />
        ))}
      </Form.Dropdown>

      <Form.Separator />

      <Form.Description text="Optional details" />

      {OPTIONAL.map((field) => (
        <Form.TextField
          key={field.name}
          id={field.name}
          title={field.title}
          placeholder={field.placeholder}
          defaultValue={text(subscription, field.name)}
        />
      ))}

      <Form.Checkbox
        id="autoRenewal"
        label="Renews automatically"
        defaultValue={subscription?.autoRenewal ?? true}
      />
    </Form>
  );
}
