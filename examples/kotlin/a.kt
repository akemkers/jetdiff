package dev.jetdiff.example

import java.time.LocalDate

/** A simple order. */
data class Order(
    val id: String,
    val quantity: Int,
    val price: Double,
)

enum class Status { NEW, SENT, REJECTED }

class OrderService(private val repo: OrderRepo) {
    private val cache = mutableMapOf<String, Order>()

    fun get(id: String): Order? = cache[id] ?: repo.find(id)

    fun sum(orders: List<Order>): Double {
        var total = 0.0
        for (o in orders) {
            total += o.quantity * o.price
        }
        return total
    }

    fun describe(o: Order): String = "Order ${o.id}: ${o.quantity} pcs"

    // Only used in tests
    fun clear() {
        cache.clear()
    }

    companion object {
        const val MAX_QUANTITY = 10_000
    }
}

interface OrderRepo {
    fun find(id: String): Order?
}
