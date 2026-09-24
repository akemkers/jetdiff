package kurs

import (
	"errors"
	"fmt"
	"time"
)

// Kurs er siste pris for et papir, med tidspunkt.
type Kurs struct {
	Ticker string
	Pris   float64
	Tid    time.Time
}

var ErrUkjent = errors.New("ukjent ticker")

func Hent(kilde map[string]float64, ticker string) (Kurs, error) {
	pris, ok := kilde[ticker]
	if !ok {
		return Kurs{}, fmt.Errorf("%w: %q", ErrUkjent, ticker)
	}
	return Kurs{Ticker: ticker, Pris: pris, Tid: time.Now()}, nil
}

func (k Kurs) String() string {
	return fmt.Sprintf("%s: %.2f (%s)", k.Ticker, k.Pris, k.Tid.Format(time.TimeOnly))
}

func Snitt(kurser []Kurs) float64 {
	if len(kurser) == 0 {
		return 0
	}
	sum := 0.0
	for _, k := range kurser {
		sum += k.Pris
	}
	return sum / float64(len(kurser))
}
