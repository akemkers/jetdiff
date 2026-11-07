"""Quotes for securities."""

from dataclasses import dataclass

MAX_AGE = 60


@dataclass
class Quote:
    """The last price for a security."""

    ticker: str
    price: float

    def __str__(self) -> str:
        return f"{self.ticker}: {self.price:.2f}"


class UnknownTicker(Exception):
    pass


def fetch(source: dict[str, float], ticker: str) -> Quote:
    if ticker not in source:
        raise UnknownTicker(ticker)
    return Quote(ticker, source[ticker])


def average(quotes: list[Quote]) -> float:
    total = 0.0
    for q in quotes:
        total += q.price
    return total / len(quotes)
