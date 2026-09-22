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
    enum class WritePreference {
        Automatic,
        WithResponse,
        WithoutResponse,
    };
    Q_ENUM(WritePreference)

    explicit BleTransport(QObject* parent = nullptr);
    explicit BleTransport(QBluetoothDeviceInfo device, QObject* parent = nullptr);
    ~BleTransport() override;

    [[nodiscard]] QString displayName() const override;
    [[nodiscard]] State state() const noexcept override;
    [[nodiscard]] model::EvidenceStatus observationEvidence() const noexcept override;
    [[nodiscard]] QString observationSource() const override;

    void startScan(int timeoutMs = flow8ApkScanTimeoutMs());
    void stopScan();
    void setDevice(const QBluetoothDeviceInfo& device);
    [[nodiscard]] const QBluetoothDeviceInfo& device() const noexcept;

    void connectTransport() override;
    void disconnectTransport() override;
    bool send(const QByteArray& payload) override;
    void protocolSessionReady() override;
    void markHandshakeClientSent();

    [[nodiscard]] bool read();
    [[nodiscard]] bool write(const QByteArray& payload, bool withoutResponse);
    [[nodiscard]] bool subscribeNotifications(bool enabled = true);
    void setAutomaticNotificationSubscription(bool enabled) noexcept;
    [[nodiscard]] bool automaticNotificationSubscription() const noexcept;
    [[nodiscard]] bool notificationsEnabled() const noexcept;
    void setWritePreference(WritePreference preference) noexcept;
    [[nodiscard]] WritePreference writePreference() const noexcept;
    [[nodiscard]] int negotiatedMtu() const noexcept;
    void setAutomaticReconnect(bool enabled, int delayMs = 1'500);

signals:
    void deviceDiscovered(const QBluetoothDeviceInfo& device, bool flow8Candidate);
    void scanFinished();
    void serviceDiscovered(const QBluetoothUuid& uuid);
    void characteristicDiscovered(const QBluetoothUuid& service,
                                  const QBluetoothUuid& characteristic,
                                  QLowEnergyCharacteristic::PropertyTypes properties);
    void descriptorDiscovered(const QBluetoothUuid& service,
                              const QBluetoothUuid& characteristic,
                              const QBluetoothUuid& descriptor);
    void gattReady();
    void notificationSubscriptionChanged(bool enabled, const QString& mode);
    void negotiatedMtuChanged(int mtu);
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
    bool automaticNotificationSubscription_ {true};
    bool notificationsEnabled_ {};
    bool manualDisconnect_ {false};
    WritePreference writePreference_ {WritePreference::Automatic};
    int negotiatedMtu_ {23};
};

} // namespace flow8::ble
