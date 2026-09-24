const API = "https://example.com/api/v2";
const TIMEOUT_MS = 5000;

/**
 * Henter kurser for en liste tickere.
 */
async function hentKurser(tickere, { signal } = {}) {
  const url = `${API}/kurser?t=${encodeURIComponent(tickere.join(","))}`;
  const svar = await fetch(url, { signal: signal ?? AbortSignal.timeout(TIMEOUT_MS) });
  if (!svar.ok) throw new Error(`HTTP ${svar.status}`);
  return svar.json();
}

function formater(kurs, valuta = "kr") {
  return `${kurs.toFixed(2)} ${valuta}`;
}

module.exports = { hentKurser, formater };
