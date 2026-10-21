package quote

import (
	"errors"
	"fmt"
)

// Quote is the last price for a security.
type Quote struct {
	Ticker string
	Price  float64
}

var ErrUnknown = errors.New("unknown ticker")

func Fetch(source map[string]float64, ticker string) (Quote, error) {
	price, ok := source[ticker]
	if !ok {
		return Quote{}, ErrUnknown
	}
	return Quote{Ticker: ticker, Price: price}, nil
}

func (q Quote) String() string {
	return fmt.Sprintf("%s: %.2f", q.Ticker, q.Price)
}

func Average(quotes []Quote) float64 {
	sum := 0.0
	for _, q := range quotes {
		sum += q.Price
	}
	return sum / float64(len(quotes))
}
