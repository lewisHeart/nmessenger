# Notificações Android sem serviço externo

Após entrar, o NMessenger inicia um serviço Android de primeiro plano que
recebe eventos em tempo real pelo WebSocket do próprio servidor. Ao conectar,
reconectar e receber um evento, ele consulta as conversas não lidas. Uma
consulta de segurança também ocorre a cada cinco minutos. Não é
necessário instalar outro aplicativo nem configurar um serviço de push externo.
O Android mostra uma notificação persistente enquanto a conexão está ativa.
Cada consulta envia somente o token de sessão por HTTPS e recebe identificadores
de conversas e mensagens; o conteúdo criptografado não é enviado na notificação.

As ações de responder e marcar como lida abrem o NMessenger para executar a
operação com a sessão e a identidade criptográfica da conta. Ao sair da conta,
o serviço é encerrado e sua sessão local é removida. O Android pode impedir
qualquer execução em segundo plano quando o usuário força a parada do aplicativo;
nesse caso é necessário abri-lo novamente.

O serviço requer o servidor com a rota `/v1/notifications/unread`. A entrega
real em segundo plano depende das políticas de bateria do dispositivo e deve
ser validada em um aparelho físico; uma compilação do APK não comprova isso.
