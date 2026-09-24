const API = "https://example.com/api";

/**
 * Henter kurser for en liste tickere.
 */
async function hentKurser(tickere) {
  const svar = await fetch(API + "/kurser?t=" + tickere.join(","));
  return svar.json();
}

function formater(kurs) {
  return kurs.toFixed(2) + " kr";
}

module.exports = { hentKurser, formater };
