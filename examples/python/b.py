"""Quotes for securities."""

from dataclasses import dataclass
from statistics import mean

MAX_AGE = 120


@dataclass(frozen=True)
class Quote:
    """The last price for a security."""

    ticker: str
    price: float
    currency: str = "NOK"

    def __str__(self) -> str:
        return f"{self.ticker}: {self.price:.2f} {self.currency}"


class UnknownTicker(KeyError):
    pass


def fetch(source: dict[str, float], ticker: str) -> Quote:
    try:
        return Quote(ticker, source[ticker])
    except KeyError:
        raise UnknownTicker(ticker) from None


def average(quotes: list[Quote]) -> float | None:
    if not quotes:
        return None
    return mean(q.price for q in quotes)
