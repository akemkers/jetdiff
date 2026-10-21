const API = "https://example.com/api";

/**
 * Fetches quotes for a list of tickers.
 */
async function fetchQuotes(tickers) {
  const response = await fetch(API + "/quotes?t=" + tickers.join(","));
  return response.json();
}

function format(price) {
  return price.toFixed(2) + " USD";
}

module.exports = { fetchQuotes, format };
