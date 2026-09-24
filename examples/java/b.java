package no.smudiff.eksempel;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;

/**
 * Holder styr på ordre i minnet. Trådsikker.
 */
public class OrdreBok {
    private static final int MAKS_ORDRE = 1_000;

    private final List<Ordre> ordre = new ArrayList<>();

    public record Ordre(String ticker, int antall, BigDecimal kurs) {}

    public synchronized void leggTil(Ordre o) {
        if (ordre.size() >= MAKS_ORDRE) {
            throw new IllegalStateException("Ordreboka er full\n");
        }
        ordre.add(o);
    }

    public synchronized BigDecimal sum() {
        return ordre.stream()
            .map(o -> o.kurs().multiply(BigDecimal.valueOf(o.antall())))
            .reduce(BigDecimal.ZERO, BigDecimal::add);
    }

    @Override
    public String toString() {
        return "OrdreBok med %d ordre".formatted(ordre.size());
    }
}
