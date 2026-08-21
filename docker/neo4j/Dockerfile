# Neo4j Community with the Graph Data Science plugin baked in.
#
# GDS ships in /var/lib/neo4j/products/ only in the *enterprise* image; on
# community, the NEO4J_PLUGINS entrypoint handler falls back to downloading
# ~62MB from graphdatascience.ninja on every container creation — a startup
# latency, a CI flake source, and a hard failure on restricted networks.
# Baking the jar into the products dir makes the stock entrypoint find it via
# its location glob (/var/lib/neo4j/products/neo4j-graph-data-science-*.jar)
# and install it offline, plugin default config included.
#
# GDS 2.13 is the only GDS series supported on Neo4j 5.26.
FROM neo4j:5.26.27-community

ARG GDS_VERSION=2.13.10
# --checksum pins the artifact (supply-chain integrity) and gives BuildKit a
# stable cache key, so cached builds skip the download entirely.
ADD --chown=neo4j:neo4j \
    --checksum=sha256:d3d842653fede3f83d6c7e2a6fafb73ac5b7dce98e2e234c18e755852e432e14 \
    https://graphdatascience.ninja/neo4j-graph-data-science-${GDS_VERSION}.jar \
    /var/lib/neo4j/products/neo4j-graph-data-science-${GDS_VERSION}.jar
