# LED Promposal (Leprosal)

This is a simple charlieplexed LED matrix with a total of 64 LEDs controlled by and ESP32-WROOM microcontroller. Note that LEDs are disable by reversing their polarity rather than turning to high-impedence mode because making such a transition involves high overhead in the esp-idf-hal library due to the required recreation of the `PinDriver` struct.

The setup can be seen below:

![leprosal_working](https://github.com/user-attachments/assets/78c4d3dc-79ff-495e-be78-3bff34b239a6)
![leprosal_front](https://github.com/user-attachments/assets/be72ce38-3f73-49b5-b1eb-999a61955d10)
![leprosal_back](https://github.com/user-attachments/assets/739746c2-fdec-45a4-9e4d-76a38fb121d3)
