#include "ble/ble_transport.h"

#include "ble/ble_services.h"

#include <QBluetoothDeviceDiscoveryAgent>
#include <QLowEnergyDescriptor>

namespace flow8::ble {
namespace {

bool hasSelectedDevice(const QBluetoothDeviceInfo& device)
{
    return !device.name().isNull() || !device.address().isNull() || !device.deviceUuid().isNull();
}

} // namespace

BleTransport::BleTransport(QObject* parent)
    : Flow8Transport(parent)
{
    reconnectTimer_.setSingleShot(true);
    connect(&reconnectTimer_, &QTimer::timeout, this, [this] {
        if (!manualDisconnect_ && automaticReconnect_ && hasSelectedDevice(device_)) {
            setState(State::Reconnecting);
            createController();
        }
    });
}

BleTransport::BleTransport(QBluetoothDeviceInfo device, QObject* parent)
    : BleTransport(parent)
{
    device_ = std::move(device);
}

BleTransport::~BleTransport()
{
    manualDisconnect_ = true;
    reconnectTimer_.stop();
    if (discoveryAgent_ && discoveryAgent_->isActive()) {
        discoveryAgent_->stop();
    }
    if (controller_) {
        controller_->disconnectFromDevice();
    }
}

QString BleTransport::displayName() const
{
    return device_.name().isEmpty() ? QStringLiteral("BLE") : device_.name();
}

Flow8Transport::State BleTransport::state() const noexcept
{
    return state_;
}

void BleTransport::startScan(const int timeoutMs)
{
    if (state_ != State::Disconnected && state_ != State::Error) {
        return;
    }
    if (discoveryAgent_) {
        discoveryAgent_->deleteLater();
    }
    auto* agent = new QBluetoothDeviceDiscoveryAgent(this);
    discoveryAgent_ = agent;
    agent->setLowEnergyDiscoveryTimeout(timeoutMs);
    connect(agent, &QBluetoothDeviceDiscoveryAgent::deviceDiscovered, this,
            [this](const QBluetoothDeviceInfo& info) {
                emit deviceDiscovered(info, isFlow8Candidate(info));
            });
    connect(agent, &QBluetoothDeviceDiscoveryAgent::finished, this, [this] {
        setState(State::Disconnected);
        emit scanFinished();
    });
    connect(agent, &QBluetoothDeviceDiscoveryAgent::canceled, this, [this] {
        setState(State::Disconnected);
        emit scanFinished();
    });
    connect(agent, &QBluetoothDeviceDiscoveryAgent::errorOccurred, this,
            [this, agent](QBluetoothDeviceDiscoveryAgent::Error) {
                reportError(QStringLiteral("BLE scan"), agent->errorString());
                emit scanFinished();
            });
    setState(State::Scanning);
    agent->start(QBluetoothDeviceDiscoveryAgent::LowEnergyMethod);
}

void BleTransport::stopScan()
{
    if (discoveryAgent_ && discoveryAgent_->isActive()) {
        discoveryAgent_->stop();
    }
}

void BleTransport::setDevice(const QBluetoothDeviceInfo& device)
{
    if (state_ != State::Disconnected && state_ != State::Error) {
        reportError(QStringLiteral("set BLE device"),
                    QStringLiteral("disconnect before selecting another device"));
        return;
    }
    device_ = device;
}

const QBluetoothDeviceInfo& BleTransport::device() const noexcept
{
    return device_;
}

void BleTransport::connectTransport()
{
    if (!hasSelectedDevice(device_)) {
        reportError(QStringLiteral("BLE connect"), QStringLiteral("no device selected"));
        return;
    }
    if (state_ != State::Disconnected && state_ != State::Error) {
        return;
    }
    manualDisconnect_ = false;
    reconnectTimer_.stop();
    setState(State::Connecting);
    createController();
}

void BleTransport::disconnectTransport()
{
    manualDisconnect_ = true;
    reconnectTimer_.stop();
    if (controller_) {
        controller_->disconnectFromDevice();
    } else {
        clearConnectionObjects();
        setState(State::Disconnected);
    }
}

bool BleTransport::send(const QByteArray& payload)
{
    if (!characteristic_.isValid()) {
        return false;
    }
    const auto properties = characteristic_.properties();
    if (properties.testFlag(QLowEnergyCharacteristic::Write)) {
        return write(payload, false);
    }
    if (properties.testFlag(QLowEnergyCharacteristic::WriteNoResponse)) {
        return write(payload, true);
    }
    return write(payload, false);
}

bool BleTransport::read()
{
    if (!service_ || !characteristic_.isValid()
        || !characteristic_.properties().testFlag(QLowEnergyCharacteristic::Read)) {
        return false;
    }
    emit trafficEvent(QStringLiteral("TX"), service_->serviceUuid(), characteristic_.uuid(),
                      QStringLiteral("read-request"), {});
    service_->readCharacteristic(characteristic_);
    return true;
}

bool BleTransport::write(const QByteArray& payload, const bool withoutResponse)
{
    if (!service_ || !characteristic_.isValid() || payload.isEmpty()) {
        return false;
    }
    const auto properties = characteristic_.properties();
    const auto required = withoutResponse ? QLowEnergyCharacteristic::WriteNoResponse
                                          : QLowEnergyCharacteristic::Write;
    if (!properties.testFlag(required)) {
        return false;
    }
    const auto mode = withoutResponse ? QLowEnergyService::WriteWithoutResponse
                                      : QLowEnergyService::WriteWithResponse;
    emit trafficEvent(QStringLiteral("TX"), service_->serviceUuid(), characteristic_.uuid(),
                      withoutResponse ? QStringLiteral("write-without-response")
                                      : QStringLiteral("write"),
                      payload);
    service_->writeCharacteristic(characteristic_, payload, mode);
    if (withoutResponse) {
        emit bytesWritten(payload);
    }
    return true;
}

bool BleTransport::subscribeNotifications(const bool enabled)
{
    if (!service_ || !characteristic_.isValid()) {
        return false;
    }
    const auto cccd = characteristic_.clientCharacteristicConfiguration();
    if (!cccd.isValid()) {
        return false;
    }
    QByteArray value = QLowEnergyCharacteristic::CCCDDisable;
    QString operation = QStringLiteral("unsubscribe");
    if (enabled && characteristic_.properties().testFlag(QLowEnergyCharacteristic::Notify)) {
        value = QLowEnergyCharacteristic::CCCDEnableNotification;
        operation = QStringLiteral("subscribe-notify");
    } else if (enabled
               && characteristic_.properties().testFlag(QLowEnergyCharacteristic::Indicate)) {
        value = QLowEnergyCharacteristic::CCCDEnableIndication;
        operation = QStringLiteral("subscribe-indicate");
    } else if (enabled) {
        return false;
    }
    emit trafficEvent(QStringLiteral("TX"), service_->serviceUuid(), characteristic_.uuid(),
                      operation, value);
    service_->writeDescriptor(cccd, value);
    return true;
}

void BleTransport::setAutomaticReconnect(const bool enabled, const int delayMs)
{
    automaticReconnect_ = enabled;
    reconnectTimer_.setInterval(qMax(0, delayMs));
    if (!enabled) {
        reconnectTimer_.stop();
    }
}

void BleTransport::setState(const State state)
{
    if (state_ == state) {
        return;
    }
    state_ = state;
    emit stateChanged(state_);
}

void BleTransport::createController()
{
    clearConnectionObjects();
    auto* controller = QLowEnergyController::createCentral(device_, this);
    controller_ = controller;
    connect(controller, &QLowEnergyController::connected, this, [this, controller] {
        if (controller_ != controller) {
            return;
        }
        setState(State::DiscoveringServices);
        controller->discoverServices();
    });
    connect(controller, &QLowEnergyController::serviceDiscovered, this,
            [this, controller](const QBluetoothUuid& uuid) {
                if (controller_ == controller) {
                    emit serviceDiscovered(uuid);
                }
            });
    connect(controller, &QLowEnergyController::discoveryFinished, this, [this, controller] {
        if (controller_ != controller) {
            return;
        }
        if (!controller->services().contains(flow8ServiceUuid())) {
            reportError(QStringLiteral("GATT discovery"),
                        QStringLiteral("FLOW 8 service was not found"));
            return;
        }
        service_ = controller->createServiceObject(flow8ServiceUuid(), this);
        if (!service_) {
            reportError(QStringLiteral("GATT discovery"),
                        QStringLiteral("failed to create service object"));
            return;
        }
        auto* service = service_.data();
        connect(service, &QLowEnergyService::stateChanged, this,
                &BleTransport::handleServiceDetails);
        connect(service, &QLowEnergyService::characteristicChanged, this,
                [this, service](const QLowEnergyCharacteristic& characteristic,
                                const QByteArray& value) {
                    if (service_ != service) {
                        return;
                    }
                    emit trafficEvent(QStringLiteral("RX"), service->serviceUuid(),
                                      characteristic.uuid(), QStringLiteral("notify"), value);
                    emit bytesReceived(value);
                });
        connect(service, &QLowEnergyService::characteristicRead, this,
                [this, service](const QLowEnergyCharacteristic& characteristic,
                                const QByteArray& value) {
                    if (service_ != service) {
                        return;
                    }
                    emit trafficEvent(QStringLiteral("RX"), service->serviceUuid(),
                                      characteristic.uuid(), QStringLiteral("read"), value);
                    emit bytesReceived(value);
                });
        connect(service, &QLowEnergyService::characteristicWritten, this,
                [this, service](const QLowEnergyCharacteristic& characteristic,
                                const QByteArray& value) {
                    if (service_ != service) {
                        return;
                    }
                    emit trafficEvent(QStringLiteral("RX"), service->serviceUuid(),
                                      characteristic.uuid(), QStringLiteral("write-ack"), value);
                    emit bytesWritten(value);
                });
        connect(service, &QLowEnergyService::errorOccurred, this,
                [this, service](QLowEnergyService::ServiceError error) {
                    if (service_ != service) {
                        return;
                    }
                    reportError(QStringLiteral("GATT service"),
                                QStringLiteral("service error %1").arg(static_cast<int>(error)));
                });
        service->discoverDetails();
    });
    connect(controller, &QLowEnergyController::disconnected, this,
            [this, controller] {
                if (controller_ == controller) {
                    handleControllerDisconnected();
                }
            });
    connect(controller, &QLowEnergyController::errorOccurred, this,
            [this, controller](QLowEnergyController::Error) {
                if (controller_ == controller) {
                    reportError(QStringLiteral("BLE controller"), controller->errorString());
                }
            });
    controller->connectToDevice();
}

void BleTransport::handleControllerDisconnected()
{
    clearConnectionObjects();
    setState(State::Disconnected);
    if (!manualDisconnect_ && automaticReconnect_) {
        setState(State::Reconnecting);
        reconnectTimer_.start();
    }
}

void BleTransport::handleServiceDetails(const QLowEnergyService::ServiceState serviceState)
{
    if (sender() != service_ || serviceState != QLowEnergyService::RemoteServiceDiscovered
        || !service_) {
        return;
    }
    for (const auto& found : service_->characteristics()) {
        emit characteristicDiscovered(service_->serviceUuid(), found.uuid(), found.properties());
    }
    characteristic_ = service_->characteristic(flow8CharacteristicUuid());
    if (!characteristic_.isValid()) {
        reportError(QStringLiteral("GATT discovery"),
                    QStringLiteral("FLOW 8 characteristic was not found"));
        return;
    }
    setState(State::Connected);
    if (!subscribeNotifications(true)) {
        emit errorOccurred(QStringLiteral(
            "GATT notification subscription is unavailable (properties or CCCD missing)"));
    }
}

void BleTransport::clearConnectionObjects()
{
    characteristic_ = {};
    if (service_) {
        service_->deleteLater();
        service_ = nullptr;
    }
    if (controller_) {
        controller_->deleteLater();
        controller_ = nullptr;
    }
}

void BleTransport::reportError(const QString& context, const QString& details)
{
    setState(State::Error);
    emit errorOccurred(QStringLiteral("%1: %2").arg(context, details));
}

} // namespace flow8::ble
