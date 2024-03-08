# bwHC Kafka Rest Proxy

bwHC MTB-File REST Proxy für Kafka

### Einordnung innerhalb einer DNPM-ETL-Strecke

Diese Anwendung erlaubt das Weiterleiten von REST Anfragen mit einem Request-Body und Inhalt eines bwHC MTB-Files
sowie `Content-Type` von `application/json` an einen Apache Kafka Cluster.

Verwendung im Zusammenspiel mit https://github.com/CCC-MF/etl-processor

![Modell DNPM-ETL-Strecke](docs/etl.png)

## Konfiguration

Die Anwendung lässt sich mit Umgebungsvariablen konfigurieren.

* `APP_KAFKA_SERVERS`: Zu verwendende Kafka-Bootstrap-Server als kommagetrennte Liste
* `APP_KAFKA_TOPIC`: Zu verwendendes Topic zum Warten auf neue Anfragen. Standardwert: `etl-processor_input`

## HTTP-Requests

Die folgenden Endpunkte sind verfügbar:

* **POST** `/mtbfile`: Senden eines MTB-Files
* **DELETE** `/mtbfile/:patient_id`: Löschen von Informationen zu dem Patienten

Übermittelte MTB-Files müssen erforderliche Bestandteile beinhalten, ansonsten wird die Anfrage zurückgewiesen.

Zum Löschen von Patienteninformationen wird intern ein MTB-File mit Consent-Status `REJECTED` erzeugt und weiter
geleitet. Hier ist kein Request-Body erforderlich.

Bei Erfolg enthält die Antwort enthält im HTTP-Header `X-Request-Id` die Anfrage-ID, die auch im ETL-Prozessor verwendet
wird.