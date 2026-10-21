import { useState } from "react";

type Props = {
  ticker: string;
  price: number;
};

export function QuoteCard({ ticker, price }: Props) {
  const [show, setShow] = useState(false);

  return (
    <div className="card">
      <h2>{ticker}</h2>
      <button onClick={() => setShow(!show)}>Show price</button>
      {show && <span className="price">{price}</span>}
    </div>
  );
}
