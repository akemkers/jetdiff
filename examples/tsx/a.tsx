import { useState } from "react";

type Props = {
  ticker: string;
  kurs: number;
};

export function KursKort({ ticker, kurs }: Props) {
  const [vis, setVis] = useState(false);

  return (
    <div className="kort">
      <h2>{ticker}</h2>
      <button onClick={() => setVis(!vis)}>Vis kurs</button>
      {vis && <span className="kurs">{kurs}</span>}
    </div>
  );
}
