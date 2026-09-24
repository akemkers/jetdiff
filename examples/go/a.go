package kurs

import (
	"errors"
	"fmt"
)

// Kurs er siste pris for et papir.
type Kurs struct {
	Ticker string
	Pris   float64
}

var ErrUkjent = errors.New("ukjent ticker")

func Hent(kilde map[string]float64, ticker string) (Kurs, error) {
	pris, ok := kilde[ticker]
	if !ok {
		return Kurs{}, ErrUkjent
	}
	return Kurs{Ticker: ticker, Pris: pris}, nil
}

func (k Kurs) String() string {
	return fmt.Sprintf("%s: %.2f", k.Ticker, k.Pris)
}

func Snitt(kurser []Kurs) float64 {
	sum := 0.0
	for _, k := range kurser {
		sum += k.Pris
	}
	return sum / float64(len(kurser))
}
