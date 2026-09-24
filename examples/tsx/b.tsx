import { useState } from "react";

type Props = {
  ticker: string;
  kurs: number;
  endring?: number;
};

export function KursKort({ ticker, kurs, endring = 0 }: Props) {
  const [vis, setVis] = useState(true);
  const retning = endring >= 0 ? "opp" : "ned";

  return (
    <section className="kort" aria-label={ticker}>
      <h2>{ticker}</h2>
      <button onClick={() => setVis((v) => !v)}>
        {vis ? "Skjul" : "Vis"} kurs
      </button>
      {vis && (
        <span className={`kurs ${retning}`}>
          {kurs.toFixed(2)} ({endring}%)
        </span>
      )}
    </section>
  );
}
