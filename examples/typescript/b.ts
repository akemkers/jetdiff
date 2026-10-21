import { formatNumber } from "./format";

/** A position in a portfolio. */
export interface Position {
  ticker: string;
  quantity: number;
  averagePrice: number;
  currency?: "NOK" | "USD";
}

export const MAX_POSITIONS = 250;

export function value(p: Position, price: number): number {
  return p.quantity * price;
}

export class Portfolio {
  private readonly positions = new Map<string, Position>();

  add(p: Position): void {
    if (this.positions.size >= MAX_POSITIONS) {
      throw new Error("Too many positions");
    }
    this.positions.set(p.ticker, p);
  }

  describe(): string {
    return `Portfolio with ${this.positions.size} positions\n`;
  }

  sum(prices: Record<string, number>): number {
    return [...this.positions.values()].reduce(
      (total, p) => total + value(p, prices[p.ticker] ?? p.averagePrice),
      0,
    );
  }
}
