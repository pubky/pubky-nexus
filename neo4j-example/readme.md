# Pubky skunk-work indexer production migration

We can temporarily develop using as placeholder for Neo4J DB the `indexer-migration.cypher` created with the `skunk-work/scripts/genCypher.js` .

Just mount this directory `/neo4j-example` as volume on your Neo4J docker container and run:

```
docker exec neo4j bash /neo4j-example/run-queries.sh
```
