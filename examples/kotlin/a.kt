package no.jetdiff.eksempel

import java.time.LocalDate

/** En enkel ordre. */
data class Ordre(
    val id: String,
    val antall: Int,
    val kurs: Double,
)

enum class Status { NY, SENDT, AVVIST }

class OrdreService(private val repo: OrdreRepo) {
    private val cache = mutableMapOf<String, Ordre>()

    fun hent(id: String): Ordre? = cache[id] ?: repo.finn(id)

    fun sum(ordre: List<Ordre>): Double {
        var total = 0.0
        for (o in ordre) {
            total += o.antall * o.kurs
        }
        return total
    }

    fun beskriv(o: Ordre): String = "Ordre ${o.id}: ${o.antall} stk"

    // Brukes bare i test
    fun tøm() {
        cache.clear()
    }

    companion object {
        const val MAKS_ANTALL = 10_000
    }
}

interface OrdreRepo {
    fun finn(id: String): Ordre?
}
