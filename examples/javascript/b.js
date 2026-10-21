const API = "https://example.com/api/v2";
const TIMEOUT_MS = 5000;

/**
 * Fetches quotes for a list of tickers.
 */
async function fetchQuotes(tickers, { signal } = {}) {
  const url = `${API}/quotes?t=${encodeURIComponent(tickers.join(","))}`;
  const response = await fetch(url, { signal: signal ?? AbortSignal.timeout(TIMEOUT_MS) });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  return response.json();
}

function format(price, currency = "USD") {
  return `${price.toFixed(2)} ${currency}`;
}

module.exports = { fetchQuotes, format };
