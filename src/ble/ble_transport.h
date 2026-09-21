#pragma once

#include "ble/ble_services.h"
#include "core/flow8_transport.h"

#include <QBluetoothDeviceInfo>
#include <QBluetoothUuid>
#include <QLowEnergyCharacteristic>
#include <QLowEnergyController>
#include <QLowEnergyService>
#include <QPointer>
#include <QTimer>

class QBluetoothDeviceDiscoveryAgent;

namespace flow8::ble {

class BleTransport final : public Flow8Transport {
    Q_OBJECT

public:
    explicit BleTransport(QObject* parent = nullptr);
    explicit BleTransport(QBluetoothDeviceInfo device, QObject* parent = nullptr);
    ~BleTransport() override;

    [[nodiscard]] QString displayName() const override;
    [[nodiscard]] State state() const noexcept override;

    void startScan(int timeoutMs = flow8ApkScanTimeoutMs());
    void stopScan();
    void setDevice(const QBluetoothDeviceInfo& device);
    [[nodiscard]] const QBluetoothDeviceInfo& device() const noexcept;

    void connectTransport() override;
    void disconnectTransport() override;
    bool send(const QByteArray& payload) override;

    [[nodiscard]] bool read();
    [[nodiscard]] bool write(const QByteArray& payload, bool withoutResponse);
    [[nodiscard]] bool subscribeNotifications(bool enabled = true);
    void setAutomaticReconnect(bool enabled, int delayMs = 1'500);

signals:
    void deviceDiscovered(const QBluetoothDeviceInfo& device, bool flow8Candidate);
    void scanFinished();
    void serviceDiscovered(const QBluetoothUuid& uuid);
    void characteristicDiscovered(const QBluetoothUuid& service,
                                  const QBluetoothUuid& characteristic,
                                  QLowEnergyCharacteristic::PropertyTypes properties);
    void trafficEvent(const QString& direction, const QBluetoothUuid& service,
                      const QBluetoothUuid& characteristic, const QString& operation,
                      const QByteArray& payload);

private:
    void setState(State state);
    void createController();
    void handleControllerDisconnected();
    void handleServiceDetails(QLowEnergyService::ServiceState serviceState);
    void clearConnectionObjects();
    void reportError(const QString& context, const QString& details);

    State state_ {State::Disconnected};
    QBluetoothDeviceInfo device_;
    QPointer<QBluetoothDeviceDiscoveryAgent> discoveryAgent_;
    QPointer<QLowEnergyController> controller_;
    QPointer<QLowEnergyService> service_;
    QLowEnergyCharacteristic characteristic_;
    QTimer reconnectTimer_;
    bool automaticReconnect_ {true};
    bool manualDisconnect_ {false};
};

} // namespace flow8::ble
