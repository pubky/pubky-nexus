# Pubky skunk-work indexer production migration

We can temporarily develop using as placeholder for Neo4J DB the `indexer.cypher` created with the `skunk-work/scripts/genCypher.js` .

Just mount this directory `/db-migration` as volume on your Neo4J docker container (`../docker-compose.yml`).

After create a new `.env` file from `.env-sample` and run the following command:

```
docker exec pubky-social-graph bash /db-migration/run-queries.sh
```
