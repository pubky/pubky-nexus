# Observability

Nexus exports telemetry over OTLP. For local development, use either the bundled observability stack or a separately installed SigNoz instance.

Configure the OTLP endpoint in _config.toml_:

```toml
[stack.otlp]
name = "nexusd"
endpoint = "http://localhost:4317"
```

OTLP export is disabled when `endpoint` is omitted. When configured, Nexus exports logs, traces, and metrics using `name` as the OpenTelemetry `service.name`.

## Local observability stack

The bundled stack runs independently of the database services and combines the OpenTelemetry Collector with Grafana, Tempo, Prometheus, and Loki.

```bash
docker compose -f docker/docker-compose.observability.yml up -d
```

### Alerting rules

Prometheus loads alerting rules from `docker/otel/alerts.yaml`, which is mounted into the container at `/etc/prometheus/alerts.yaml`. To use your own rules file, set `PROMETHEUS_ALERTS_FILE` in `docker/.env` (paths are resolved relative to the `docker/` folder):

```bash
PROMETHEUS_ALERTS_FILE=./otel/my-alerts.yaml
```

After editing the rules, reload Prometheus without restarting the stack:

```bash
curl -X POST http://localhost:9090/-/reload
```

Validate a rules file before mounting it:

```bash
docker run --rm -v "$PWD/docker/otel:/rules:ro" --entrypoint promtool prom/prometheus:v2.55.1 check rules /rules/alerts.yaml
```

Active alerts are listed at [http://localhost:9090/alerts](http://localhost:9090/alerts). Grafana also shows them under **Alerting > Alert rules** as data source-managed rules of the Prometheus datasource.

## SigNoz

SigNoz remains supported as an alternative OpenTelemetry backend. Follow the [SigNoz installation guide](https://signoz.io/docs/install), then replace the local endpoint above with the SigNoz OTLP endpoint. Its local dashboard is available at [http://localhost:3301](http://localhost:3301).
