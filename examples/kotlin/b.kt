package no.jetdiff.eksempel

import java.time.LocalDate
import java.math.BigDecimal

/** En enkel ordre med dato. */
data class Ordre(
    val id: String,
    val antall: Int,
    val kurs: BigDecimal,
    val dato: LocalDate = LocalDate.now(),
)

enum class Status { NY, SENDT, AVVIST }

class OrdreService(private val repo: OrdreRepo) {
    private val cache = mutableMapOf<String, Ordre>()

    fun hent(id: String): Ordre? = cache[id] ?: repo.finn(id)?.also { cache[id] = it }

    fun sum(ordre: List<Ordre>): BigDecimal =
        ordre.fold(BigDecimal.ZERO) { acc, o -> acc + o.kurs * o.antall.toBigDecimal() }

    fun beskriv(o: Ordre): String = "Ordre ${o.id}: ${o.antall} stk\t(${o.dato})"

    companion object {
        const val MAKS_ANTALL = 10_000
    }
}

interface OrdreRepo {
    fun finn(id: String): Ordre?
}
