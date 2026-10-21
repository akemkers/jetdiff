import { useState } from "react";

type Props = {
  ticker: string;
  price: number;
  change?: number;
};

export function QuoteCard({ ticker, price, change = 0 }: Props) {
  const [show, setShow] = useState(true);
  const direction = change >= 0 ? "up" : "down";

  return (
    <section className="card" aria-label={ticker}>
      <h2>{ticker}</h2>
      <button onClick={() => setShow((v) => !v)}>
        {show ? "Hide" : "Show"} price
      </button>
      {show && (
        <span className={`price ${direction}`}>
          {price.toFixed(2)} ({change}%)
        </span>
      )}
    </section>
  );
}
