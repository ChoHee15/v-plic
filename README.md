
rust-模拟plic

# 物理中断过程

1. 中断源发送信号至其对应gateway
2. gateway发送中断请求，设置IP位
3. PLIC核心给所有满足条件的target发送notification
4. target收到外部中断，并进行claim来获取ID
5. 而后PLIC核心清除对应的IP位
6. target处理完中断后，发送complete通告对应gateway
7. gateway转发同一中断源的下一个中断请求


# 模拟过程

1. 设备调用plic试图引起中断
2. 占据gateway后，设置pending位
3. 更新plic状态，看context是否有可通知的中断，若有则为对应vcpu设置相应中断（目前为直接设置hvip），并将最优中断放入claim
4. vcpu收到中断，尝试读取claim来获知是哪个设备，最终导向plic的读取
5. 读取claim后，清除对应中断源的pending，返回claim
6. vcpu获取中断源，做相应处理，并写complete，最终导向plic写入
7. 写入claim后，解除gateway，下个中断可进入


# key

1. 同一时间内每个中断源最多只能有一个中断请求被pending在plic
2. gateway只会在同一中断源的上一个请求完成后，再转发新的中断请求
3. 若target为hart context，中断最终会根据context的特权级，到达meip/seip
4. plic只支持多播，当有活动的中断时，所有enable的target都会收到notification，但只有一个hart能够claim
5. 根据平台架构和用于传输中断通知的方法，目标可能需要一些时间才能接收到这些通知。只要PLIC核心中没有干预活动，PLIC就保证最终将EIP中的所有状态变化传递给所有目标？
6. 中断通知中的值仅保证包含过去某个时间点有效的EIP值。特别地，第二目标可以在向第一目标发送通知的过程中响应并声明中断，这样当第一目标试图声明中断时，它会发现PLIC核心中没有活动的中断。











