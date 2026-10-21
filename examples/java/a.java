package dev.jetdiff.example;

import java.util.ArrayList;
import java.util.List;

/**
 * Keeps track of orders in memory.
 */
public class OrderBook {
    private static final int MAX_ORDERS = 500;

    private final List<Order> orders = new ArrayList<>();

    public record Order(String ticker, int quantity, double price) {}

    public void add(Order o) {
        orders.add(o);
    }

    public double sum() {
        double total = 0;
        for (Order o : orders) {
            total += o.quantity() * o.price();
        }
        return total;
    }

    @Override
    public String toString() {
        return "OrderBook with " + orders.size() + " orders";
    }
}
