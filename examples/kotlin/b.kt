package dev.jetdiff.example

import java.time.LocalDate
import java.math.BigDecimal

/** A simple order with a date. */
data class Order(
    val id: String,
    val quantity: Int,
    val price: BigDecimal,
    val date: LocalDate = LocalDate.now(),
)

enum class Status { NEW, SENT, REJECTED }

class OrderService(private val repo: OrderRepo) {
    private val cache = mutableMapOf<String, Order>()

    fun get(id: String): Order? = cache[id] ?: repo.find(id)?.also { cache[id] = it }

    fun sum(orders: List<Order>): BigDecimal =
        orders.fold(BigDecimal.ZERO) { acc, o -> acc + o.price * o.quantity.toBigDecimal() }

    fun describe(o: Order): String = "Order ${o.id}: ${o.quantity} pcs\t(${o.date})"

    companion object {
        const val MAX_QUANTITY = 10_000
    }
}

interface OrderRepo {
    fun find(id: String): Order?
}
