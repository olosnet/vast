For interface we use a message queue system

interface is subdivided in sections


from http you send a request you append request in queue

when queue is processed update interface states

server seds updates to websocket

interface listens to websocket updates

when interface receives update download new state and update interface
