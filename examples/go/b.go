package quote

import (
	"errors"
	"fmt"
	"time"
)

// Quote is the last price for a security, with a timestamp.
type Quote struct {
	Ticker string
	Price  float64
	Time   time.Time
}

var ErrUnknown = errors.New("unknown ticker")

func Fetch(source map[string]float64, ticker string) (Quote, error) {
	price, ok := source[ticker]
	if !ok {
		return Quote{}, fmt.Errorf("%w: %q", ErrUnknown, ticker)
	}
	return Quote{Ticker: ticker, Price: price, Time: time.Now()}, nil
}

func (q Quote) String() string {
	return fmt.Sprintf("%s: %.2f (%s)", q.Ticker, q.Price, q.Time.Format(time.TimeOnly))
}

func Average(quotes []Quote) float64 {
	if len(quotes) == 0 {
		return 0
	}
	sum := 0.0
	for _, q := range quotes {
		sum += q.Price
	}
	return sum / float64(len(quotes))
}
