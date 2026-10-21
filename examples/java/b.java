package dev.jetdiff.example;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;

/**
 * Keeps track of orders in memory. Thread-safe.
 */
public class OrderBook {
    private static final int MAX_ORDERS = 1_000;

    private final List<Order> orders = new ArrayList<>();

    public record Order(String ticker, int quantity, BigDecimal price) {}

    public synchronized void add(Order o) {
        if (orders.size() >= MAX_ORDERS) {
            throw new IllegalStateException("The order book is full\n");
        }
        orders.add(o);
    }

    public synchronized BigDecimal sum() {
        return orders.stream()
            .map(o -> o.price().multiply(BigDecimal.valueOf(o.quantity())))
            .reduce(BigDecimal.ZERO, BigDecimal::add);
    }

    @Override
    public String toString() {
        return "OrderBook with %d orders".formatted(orders.size());
    }
}
