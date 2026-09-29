/**
 * Types mirroring the JSON that `subtrack <cmd> --json` writes to stdout.
 *
 * These track `SharedArgs` in packages/subtrack/src/types.ts. Keep them in sync —
 * `npm run typecheck` will not catch drift, so treat a subtrack schema change as
 * an extension change too.
 */

export type Currency = string;

export type Status = "active" | "paused" | "cancelled" | "archived";

export type Cycle =
  "weekly" | "bi-weekly" | "monthly" | "quarterly" | "semi-annual" | "yearly";

export type DiscountType = "percentage" | "fixed" | null;

/** A subscription as returned by `list`, `search`, and `summary.mostExpensive`. */
export type Subscription = {
  id: number;
  name: string;
  price: number;
  currency: Currency;
  cycle: Cycle;
  status: Status;
  billingDay: number | null;
  createdAt: string;
  notes: string | null;
  paymentMethod: string | null;
  contractStart: string | null;
  contractEnd: string | null;
  autoRenewal: boolean;
  vendorName: string | null;
  vendorUrl: string | null;
  planTier: string | null;
  discountAmount: number | null;
  discountType: DiscountType;
  tags: string[];
};

/** An entry as returned by `upcoming --json`. */
export type UpcomingEntry = {
  id: number;
  name: string;
  price: number;
  currency: Currency;
  cycle: Cycle;
  nextDate: string;
  amount: number;
  tags: string[];
};

/** A row as returned by `payment --json`. */
export type PaymentRow = {
  id: number;
  name: string;
  price: number;
  currency: Currency;
  cycle: Cycle;
  status: Status;
  periodPrice: number;
};

export type SummaryData = {
  totalCount: number;
  monthlyByCurrency: Record<Currency, number>;
  monthlyByTag: Record<
    string,
    { count: number; monthly: Record<Currency, number> }
  >;
  mostExpensive?: Subscription;
};

export type PaymentData = {
  period: string;
  total: number;
  currency?: Currency | null;
  subscriptions: PaymentRow[];
};

/**
 * `budget --json` omits every amount when no budget is configured, so the two
 * shapes are modelled separately. The CLI resolves `spending`, `remaining`, and
 * `over` itself; `currency` follows `--currency` and matches `budgetCurrency`.
 */
export type BudgetData =
  | { set: false; period: string; budgetName: string | null }
  | {
      set: true;
      period: string;
      budgetName: string | null;
      budget: number;
      budgetCurrency: Currency;
      spending: number;
      currency: Currency;
      remaining: number;
      over: boolean;
    };
