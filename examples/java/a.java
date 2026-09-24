package no.smudiff.eksempel;

import java.util.ArrayList;
import java.util.List;

/**
 * Holder styr på ordre i minnet.
 */
public class OrdreBok {
    private static final int MAKS_ORDRE = 500;

    private final List<Ordre> ordre = new ArrayList<>();

    public record Ordre(String ticker, int antall, double kurs) {}

    public void leggTil(Ordre o) {
        ordre.add(o);
    }

    public double sum() {
        double total = 0;
        for (Ordre o : ordre) {
            total += o.antall() * o.kurs();
        }
        return total;
    }

    @Override
    public String toString() {
        return "OrdreBok med " + ordre.size() + " ordre";
    }
}
